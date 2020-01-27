package de.sve.backend.model;

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
			String titleColor, List<LocalDateTime> dates, Long durationInMinutes, Long maxSubscribers, Long subscribers, Double costMember, Double costNonMember, Long waitingList, Long maxWaitingList,
			String location, String bookingTemplate, String waitingTemplate) {
		return new AutoValue_Event(id, sheetId, gid, type, name, sortIndex, visible, shortDescription, description, image, titleColor, dates, durationInMinutes, maxSubscribers, subscribers,
				costMember, costNonMember, waitingList, maxWaitingList, location, bookingTemplate, waitingTemplate);
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
	public abstract String titleColor();

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

	public boolean isBookedUp() {
		return subscribers() >= maxSubscribers() && waitingList() >= maxWaitingList();
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
					  firstNonNull(event.titleColor(), titleColor()),
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
					  firstNonNull(event.waitingTemplate(), waitingTemplate()));
	}

}
