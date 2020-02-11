package de.sve.backend.model.events;

import static com.google.common.base.MoreObjects.firstNonNull;

import java.time.LocalDateTime;
import java.util.List;

import javax.annotation.Nullable;

import com.google.auto.value.AutoValue;
import com.ryanharter.auto.value.gson.GenerateTypeAdapter;

@AutoValue
@GenerateTypeAdapter
public abstract class Event {

	public static Event create(String id, String sheetId, Long gid, EventType type, String name, Long sortIndex, Boolean visible, String shortDescription, String description, String image,
			Boolean light, List<LocalDateTime> dates, Long durationInMinutes, Long maxSubscribers, Long subscribers, Double costMember, Double costNonMember, Long waitingList, Long maxWaitingList,
			String location, String bookingTemplate, String waitingTemplate, Boolean externalOperator) {
		return new AutoValue_Event(id, sheetId, gid, type, name, sortIndex, visible, shortDescription, description, image, light, dates, durationInMinutes, maxSubscribers, subscribers, costMember,
				costNonMember, waitingList, maxWaitingList, location, bookingTemplate, waitingTemplate, externalOperator);
	}

	public abstract String id();

	@Nullable
	public abstract String sheetId();

	@Nullable
	public abstract Long gid();

	@Nullable
	public abstract EventType type();

	@Nullable
	public abstract String name();

	@Nullable
	public abstract Long sortIndex();

	@Nullable
	public abstract Boolean visible();

	@Nullable
	public abstract String shortDescription();

	@Nullable
	public abstract String description();

	@Nullable
	public abstract String image();

	@Nullable
	public abstract Boolean light();

	@Nullable
	public abstract List<LocalDateTime> dates();

	@Nullable
	public abstract Long durationInMinutes();

	@Nullable
	public abstract Long maxSubscribers();

	@Nullable
	public abstract Long subscribers();

	@Nullable
	public abstract Double costMember();

	@Nullable
	public abstract Double costNonMember();

	@Nullable
	public abstract Long waitingList();

	@Nullable
	public abstract Long maxWaitingList();

	@Nullable
	public abstract String location();

	@Nullable
	public abstract String bookingTemplate();

	@Nullable
	public abstract String waitingTemplate();

	@Nullable
	public abstract Boolean externalOperator();

	public boolean isBookedUp() {
		return subscribers() >= maxSubscribers() && waitingList() >= maxWaitingList();
	}

	public Event bookEvent() {
		Long subscribers = subscribers();
		Long waitingList = waitingList();
		if (subscribers() < maxSubscribers()) {
			subscribers = Long.valueOf(subscribers.longValue() + 1);
		} else if (waitingList < maxWaitingList()) {
			waitingList = Long.valueOf(waitingList.longValue() + 1);
		}
		return create(id(), sheetId(), gid(), type(), name(), sortIndex(), visible(), shortDescription(), description(), image(), light(), dates(), durationInMinutes(), maxSubscribers(), subscribers,
				costMember(), costNonMember(), waitingList, maxWaitingList(), location(), bookingTemplate(), waitingTemplate(), externalOperator());
	}

	public Event update(Event event) {
		return create(id(),
					  firstNonNull(event.sheetId(), sheetId()),
					  firstNonNull(event.gid(), gid()),
					  firstNonNull(event.type(), type()),
					  firstNonNull(event.name(), name()),
					  firstNonNull(event.sortIndex(), sortIndex()),
					  firstNonNull(event.visible(), visible()),
					  firstNonNull(event.shortDescription(), shortDescription()),
					  firstNonNull(event.description(), description()),
					  firstNonNull(event.image(), image()),
					  firstNonNull(event.light(), light()),
					  firstNonNull(event.dates(), dates()),
					  firstNonNull(event.durationInMinutes(), durationInMinutes()),
					  firstNonNull(event.maxSubscribers(), maxSubscribers()),
					  firstNonNull(event.subscribers(), subscribers()),
					  firstNonNull(event.costMember(), costMember()),
					  firstNonNull(event.costNonMember(), costNonMember()),
					  firstNonNull(event.waitingList(), waitingList()),
					  firstNonNull(event.maxWaitingList(), maxWaitingList()),
					  firstNonNull(event.location(), location()),
					  firstNonNull(event.bookingTemplate(), bookingTemplate()),
					  firstNonNull(event.waitingTemplate(), waitingTemplate()),
					  firstNonNull(event.externalOperator(), externalOperator()));
	}

}