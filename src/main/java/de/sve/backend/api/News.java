package de.sve.backend.api;

import java.util.Collections;
import java.util.Map;
import java.util.Set;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import com.google.common.base.MoreObjects;

import de.sve.backend.api.utils.BackendException;
import de.sve.backend.manager.NewsManager;
import de.sve.backend.model.news.NewsType;
import de.sve.backend.model.news.Subscription;
import jakarta.ws.rs.GET;
import jakarta.ws.rs.POST;
import jakarta.ws.rs.Path;
import jakarta.ws.rs.Produces;
import jakarta.ws.rs.core.MediaType;

@Path("/news")
@SuppressWarnings("static-method")
public class News {

	private static final Logger LOG = LoggerFactory.getLogger(News.class);

	@Path("/subscribe")
	@POST
	@Produces(MediaType.APPLICATION_JSON)
	public void subscribe(Subscription subscription) throws BackendException {
		try {
			NewsManager.subscribe(subscription);
		} catch (Throwable t) {
			String message = "Error while subscribe to news: " + subscription; //$NON-NLS-1$
			LOG.error(message, t);
			throw new BackendException(message, t);
		}
	}

	@Path("/unsubscribe")
	@POST
	@Produces(MediaType.APPLICATION_JSON)
	public void unsubscribe(Subscription subscription) throws BackendException {
		try {
			NewsManager.unsubscribe(subscription);
		} catch (Throwable t) {
			String message = "Error while unsubscribe from news: " + subscription; //$NON-NLS-1$
			LOG.error(message, t);
			throw new BackendException(message, t);
		}
	}

	@Path("/subscribers")
	@GET
	@Produces(MediaType.TEXT_HTML)
	public String subscribers() throws Exception {
		StringBuilder builder = new StringBuilder();
		Map<NewsType, Set<String>> subscriptions = NewsManager.subscriptions();
		for (NewsType type : NewsType.values()) {
			Set<String> emails = MoreObjects.firstNonNull(subscriptions.get(type), Collections.emptySet());
			if (builder.length() > 0) {
				builder.append("<br/><br/><br/>"); //$NON-NLS-1$
			}
			String title = "---------- " + type.displayName() + ":" + emails.size() + " ----------"; //$NON-NLS-1$ //$NON-NLS-2$ //$NON-NLS-3$
			builder.append(title);
			builder.append("<br/>"); //$NON-NLS-1$
			builder.append(String.join(";", emails)); //$NON-NLS-1$
			builder.append("<br/>"); //$NON-NLS-1$
			builder.append(title);
		}
		return builder.toString();
	}

}
