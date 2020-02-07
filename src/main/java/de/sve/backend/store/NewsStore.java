package de.sve.backend.store;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ExecutionException;
import java.util.stream.Collectors;

import com.google.api.core.ApiFuture;
import com.google.cloud.firestore.CollectionReference;
import com.google.cloud.firestore.DocumentSnapshot;
import com.google.cloud.firestore.Firestore;
import com.google.cloud.firestore.FirestoreOptions;
import com.google.cloud.firestore.QueryDocumentSnapshot;
import com.google.cloud.firestore.QuerySnapshot;

import de.sve.backend.model.news.NewsType;
import de.sve.backend.model.news.Subscription;

public class NewsStore implements AutoCloseable {

	private Firestore fService;

	protected NewsStore() {
		this.fService = FirestoreOptions.getDefaultInstance().getService();
	}

	protected List<Subscription> loadSubscriptions() throws InterruptedException, ExecutionException {
		ApiFuture<QuerySnapshot> future = collection().get();
		QuerySnapshot querySnapshot = future.get();
		List<Subscription> subscriptions = new ArrayList<>();
		for (QueryDocumentSnapshot document : querySnapshot.getDocuments()) {
			subscriptions.add(create(document));
		}
		return subscriptions;
	}

	protected Subscription loadSubscription(Subscription subscription) throws InterruptedException, ExecutionException {
		ApiFuture<DocumentSnapshot> future = collection().document(subscription.email()).get();
		DocumentSnapshot document = future.get();
		if (document.exists()) {
			return create(document);
		}
		return null;
	}

	protected void saveSubscription(Subscription subscription) throws InterruptedException, ExecutionException {
		List<String> types = subscription.types()
								  		 .stream()
								  		 .map(NewsType::toString)
								  		 .collect(Collectors.toList());
		Map<String, Object> data = new HashMap<>();
		data.put("types", types); //$NON-NLS-1$
		collection().document(subscription.email()).set(data).get();
	}

	protected void deleteSubsription(Subscription subscription) throws InterruptedException, ExecutionException {
		collection().document(subscription.email()).delete().get();
	}

	private CollectionReference collection() {
		return this.fService.collection("subscriptions"); //$NON-NLS-1$
	}

	@Override
	public void close() throws Exception {
		this.fService.close();
	}

	private static final Subscription create(DocumentSnapshot document) {
		@SuppressWarnings("unchecked")
		List<String> types = (List<String>) document.get("types"); //$NON-NLS-1$
		return Subscription.create(document.getId(), 
								   types.stream()
								   		.map(NewsType::valueOf)
								   		.collect(Collectors.toSet()));
	}

}
